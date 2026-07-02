package ai.nubase.auth.service;

import ai.nubase.auth.dto.oauth.OAuthUserInfo;
import ai.nubase.auth.dto.response.AuthResponse;
import ai.nubase.auth.entity.FlowState;
import ai.nubase.auth.entity.MfaAmrClaim;
import ai.nubase.auth.entity.SamlProvider;
import ai.nubase.auth.entity.SamlRelayState;
import ai.nubase.auth.entity.SsoDomain;
import ai.nubase.auth.entity.SsoProvider;
import ai.nubase.auth.entity.User;
import ai.nubase.auth.repository.FlowStateRepository;
import ai.nubase.auth.repository.SamlProviderRepository;
import ai.nubase.auth.repository.SamlRelayStateRepository;
import ai.nubase.auth.repository.SsoDomainRepository;
import ai.nubase.auth.repository.SsoProviderRepository;
import ai.nubase.auth.util.TokenGenerator;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

import javax.xml.crypto.dsig.XMLSignature;
import javax.xml.crypto.dsig.XMLSignatureFactory;
import javax.xml.crypto.dsig.dom.DOMValidateContext;
import javax.xml.parsers.DocumentBuilder;
import javax.xml.parsers.DocumentBuilderFactory;
import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.security.PublicKey;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.time.Instant;
import java.util.Base64;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.zip.Deflater;
import java.util.zip.DeflaterOutputStream;

/**
 * SAML 2.0 Service Provider (SP-initiated SSO).
 *
 * <p>Implements the HTTP-Redirect binding for the AuthnRequest and the HTTP-POST binding for
 * the IdP response (ACS). Response/assertion signatures are validated against the IdP X.509
 * certificate configured on the {@link SamlProvider}, using the JDK's XML Digital Signature
 * API (no external SAML library). Mirrors the surface of Supabase GoTrue's SSO endpoints.
 */
@Service
@RequiredArgsConstructor
@Slf4j
public class SamlService {

    private static final String SAMLP_NS = "urn:oasis:names:tc:SAML:2.0:protocol";
    private static final String SAML_NS = "urn:oasis:names:tc:SAML:2.0:assertion";
    private static final String DSIG_NS = "http://www.w3.org/2000/09/xmldsig#";

    private final SsoProviderRepository ssoProviderRepository;
    private final SsoDomainRepository ssoDomainRepository;
    private final SamlProviderRepository samlProviderRepository;
    private final SamlRelayStateRepository samlRelayStateRepository;
    private final FlowStateRepository flowStateRepository;
    private final OAuthService oauthService;
    private final PkceService pkceService;
    private final AuthResponseFactory authResponseFactory;
    private final AuditService auditService;
    private final TokenGenerator tokenGenerator;
    private final AuthConfig authConfig;

    /** Outcome of an ACS callback: where to send the browser + the issued credentials. */
    public record SamlLoginResult(String redirectTo, AuthResponse session, String pkceAuthCode) {
        public boolean isPkce() {
            return pkceAuthCode != null;
        }
    }

    // ---------------------------------------------------------------- SP-initiated start

    /**
     * Begin SSO. Resolve the provider (by email domain or provider id), build a signed-free
     * AuthnRequest and return the IdP redirect URL.
     */
    @Transactional
    public String initiate(String domain, String providerId, String email, String redirectTo,
                           String codeChallenge, String codeChallengeMethod) {
        SsoProvider provider = resolveProvider(domain, providerId, email);
        SamlProvider saml = samlProviderRepository.findBySsoProviderId(provider.getId())
                .orElseThrow(() -> new RuntimeException("SAML configuration not found for SSO provider"));
        if (StringUtils.isBlank(saml.getSsoUrl())) {
            throw new RuntimeException("SAML provider has no SSO URL configured");
        }

        String requestId = "_" + UUID.randomUUID().toString().replace("-", "");
        String relayToken = tokenGenerator.generateSecureToken();

        // Optionally pre-create a PKCE flow placeholder so the eventual auth_code can be exchanged.
        FlowState flow = null;
        if (StringUtils.isNotBlank(codeChallenge)) {
            String method = StringUtils.isNotBlank(codeChallengeMethod)
                    ? codeChallengeMethod : FlowState.METHOD_S256;
            flow = flowStateRepository.save(FlowState.builder()
                    .authCode("pending-" + relayToken)        // replaced with a real code at ACS time
                    .codeChallenge(codeChallenge)
                    .codeChallengeMethod(method)
                    .providerType("saml")
                    .authenticationMethod("sso:saml")
                    .build());
        }

        samlRelayStateRepository.save(SamlRelayState.builder()
                .ssoProvider(provider)
                .requestId(requestId)
                .forEmail(email)
                .redirectTo(redirectTo)
                .flowState(flow)
                .build());

        String authnRequest = buildAuthnRequest(requestId, saml.getSsoUrl());
        String encoded = deflateAndEncode(authnRequest);
        String sep = saml.getSsoUrl().contains("?") ? "&" : "?";
        return saml.getSsoUrl() + sep
                + "SAMLRequest=" + urlEncode(encoded)
                + "&RelayState=" + urlEncode(relayToken);
    }

    // ---------------------------------------------------------------- ACS (IdP response)

    /**
     * Handle the IdP SAML Response posted to the ACS endpoint: validate, extract the user,
     * find/create the account and produce a session (or a PKCE auth code).
     */
    @Transactional
    public SamlLoginResult handleAcs(String samlResponseB64, String relayState) {
        Document doc = parse(Base64.getMimeDecoder().decode(samlResponseB64));
        Element root = doc.getDocumentElement();

        // Match the in-flight request by InResponseTo to find provider + redirect + flow.
        String inResponseTo = root.getAttribute("InResponseTo");
        SamlRelayState relay = StringUtils.isNotBlank(inResponseTo)
                ? samlRelayStateRepository.findByRequestId(inResponseTo).orElse(null)
                : null;
        if (relay == null) {
            throw new RuntimeException("No matching SAML request (InResponseTo) found");
        }
        SsoProvider provider = relay.getSsoProvider();
        SamlProvider saml = samlProviderRepository.findBySsoProviderId(provider.getId())
                .orElseThrow(() -> new RuntimeException("SAML configuration not found"));

        // Verify the XML signature against the configured IdP certificate.
        verifySignature(doc, saml);

        // Extract the assertion + subject/email/attributes.
        Element assertion = firstElement(doc, SAML_NS, "Assertion");
        if (assertion == null) {
            throw new RuntimeException("SAML response contains no assertion");
        }
        validateConditions(assertion);
        Map<String, Object> attributes = extractAttributes(assertion);
        String email = resolveEmail(assertion, attributes, saml);
        if (StringUtils.isBlank(email)) {
            throw new RuntimeException("SAML assertion did not yield an email");
        }

        // Build provider user info and upsert the user (provider id namespaced per SSO provider).
        String providerName = "sso:" + provider.getId();
        String nameId = textOf(firstElement(assertion, SAML_NS, "NameID"));
        OAuthUserInfo info = OAuthUserInfo.builder()
                .provider(providerName)
                .providerId(StringUtils.isNotBlank(nameId) ? nameId : email)
                .email(email)
                .emailVerified(true)
                .name(stringAttr(attributes, "name", "displayName", "cn"))
                .build();

        OAuthService.ProviderSignIn signIn = oauthService.signInWithProviderInfo(info, relay.getFlowState() == null);
        User user = signIn.user();
        auditService.record(AuditService.LOGIN, user, Map.of("provider", providerName));

        String redirectTo = relay.getRedirectTo();
        samlRelayStateRepository.delete(relay);

        // PKCE: promote the placeholder flow to a real auth code.
        if (relay.getFlowState() != null) {
            FlowState flow = relay.getFlowState();
            String authCode = pkceService.issueAuthCode(
                    user, flow.getCodeChallenge(), flow.getCodeChallengeMethod(), "saml", "sso:saml");
            flowStateRepository.delete(flow);  // remove the placeholder; issueAuthCode created the real one
            return new SamlLoginResult(redirectTo, null, authCode);
        }

        return new SamlLoginResult(redirectTo, signIn.response(), null);
    }

    // ---------------------------------------------------------------- SP metadata

    /** Build the SP metadata XML advertised to IdP administrators. */
    public String spMetadata() {
        String entityId = spEntityId();
        String acsUrl = acsUrl();
        return """
                <?xml version="1.0" encoding="UTF-8"?>
                <md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="%s">
                  <md:SPSSODescriptor AuthnRequestsSigned="false" WantAssertionsSigned="true"
                      protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
                    <md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>
                    <md:AssertionConsumerService index="0" isDefault="true"
                        Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST" Location="%s"/>
                  </md:SPSSODescriptor>
                </md:EntityDescriptor>
                """.formatted(entityId, acsUrl);
    }

    // ---------------------------------------------------------------- admin CRUD

    /** Register a SAML SSO provider (provider + domains + SAML config). */
    @Transactional
    public ai.nubase.auth.dto.response.sso.SsoProviderResponse createProvider(
            ai.nubase.auth.dto.request.sso.CreateSsoProviderRequest req) {
        if (req.getEntityId() == null || (req.getSsoUrl() == null && req.getMetadataUrl() == null)) {
            throw new IllegalArgumentException("entity_id and sso_url (or metadata_url) are required");
        }
        SsoProvider provider = ssoProviderRepository.save(SsoProvider.builder()
                .resourceId(req.getResourceId())
                .enabled(true)
                .build());

        if (req.getDomains() != null) {
            for (String d : req.getDomains()) {
                ssoDomainRepository.save(SsoDomain.builder()
                        .ssoProvider(provider)
                        .domain(d)
                        .build());
            }
        }

        samlProviderRepository.save(SamlProvider.builder()
                .ssoProvider(provider)
                .entityId(req.getEntityId())
                .metadataXml(req.getMetadataXml())
                .metadataUrl(req.getMetadataUrl())
                .ssoUrl(req.getSsoUrl())
                .x509Certificate(req.getX509Certificate())
                .attributeMapping(req.getAttributeMapping())
                .build());

        return toResponse(provider);
    }

    @Transactional(readOnly = true)
    public java.util.List<ai.nubase.auth.dto.response.sso.SsoProviderResponse> listProviders() {
        return ssoProviderRepository.findAll().stream().map(this::toResponse).toList();
    }

    @Transactional(readOnly = true)
    public ai.nubase.auth.dto.response.sso.SsoProviderResponse getProvider(String id) {
        return toResponse(ssoProviderRepository.findById(UUID.fromString(id))
                .orElseThrow(() -> new RuntimeException("SSO provider not found")));
    }

    @Transactional
    public void deleteProvider(String id) {
        ssoProviderRepository.deleteById(UUID.fromString(id));
    }

    private ai.nubase.auth.dto.response.sso.SsoProviderResponse toResponse(SsoProvider provider) {
        SamlProvider saml = samlProviderRepository.findBySsoProviderId(provider.getId()).orElse(null);
        java.util.List<String> domains = ssoDomainRepository.findBySsoProviderId(provider.getId())
                .stream().map(SsoDomain::getDomain).toList();
        return ai.nubase.auth.dto.response.sso.SsoProviderResponse.builder()
                .id(provider.getId().toString())
                .resourceId(provider.getResourceId())
                .enabled(Boolean.TRUE.equals(provider.getEnabled()))
                .domains(domains)
                .entityId(saml != null ? saml.getEntityId() : null)
                .ssoUrl(saml != null ? saml.getSsoUrl() : null)
                .createdAt(provider.getCreatedAt())
                .build();
    }

    // ---------------------------------------------------------------- helpers

    private SsoProvider resolveProvider(String domain, String providerId, String email) {
        if (StringUtils.isNotBlank(providerId)) {
            return ssoProviderRepository.findById(UUID.fromString(providerId))
                    .orElseThrow(() -> new RuntimeException("SSO provider not found"));
        }
        String lookupDomain = domain;
        if (StringUtils.isBlank(lookupDomain) && StringUtils.isNotBlank(email) && email.contains("@")) {
            lookupDomain = email.substring(email.indexOf('@') + 1);
        }
        if (StringUtils.isBlank(lookupDomain)) {
            throw new IllegalArgumentException("One of domain, provider_id or email is required");
        }
        final String resolvedDomain = lookupDomain;
        SsoDomain ssoDomain = ssoDomainRepository.findByDomainIgnoreCase(resolvedDomain)
                .orElseThrow(() -> new RuntimeException("No SSO provider registered for domain: " + resolvedDomain));
        return ssoDomain.getSsoProvider();
    }

    private String buildAuthnRequest(String requestId, String destination) {
        String issueInstant = Instant.now().toString();
        return ("<samlp:AuthnRequest xmlns:samlp=\"" + SAMLP_NS + "\" xmlns:saml=\"" + SAML_NS + "\" "
                + "ID=\"" + requestId + "\" Version=\"2.0\" IssueInstant=\"" + issueInstant + "\" "
                + "Destination=\"" + destination + "\" "
                + "ProtocolBinding=\"urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST\" "
                + "AssertionConsumerServiceURL=\"" + acsUrl() + "\">"
                + "<saml:Issuer>" + spEntityId() + "</saml:Issuer>"
                + "<samlp:NameIDPolicy Format=\"urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress\" AllowCreate=\"true\"/>"
                + "</samlp:AuthnRequest>");
    }

    private String deflateAndEncode(String xml) {
        try {
            java.io.ByteArrayOutputStream bos = new java.io.ByteArrayOutputStream();
            Deflater deflater = new Deflater(Deflater.DEFAULT_COMPRESSION, true); // raw DEFLATE (no zlib header)
            try (DeflaterOutputStream dos = new DeflaterOutputStream(bos, deflater)) {
                dos.write(xml.getBytes(StandardCharsets.UTF_8));
            }
            return Base64.getEncoder().encodeToString(bos.toByteArray());
        } catch (Exception e) {
            throw new RuntimeException("Failed to encode SAML request", e);
        }
    }

    Document parse(byte[] xml) {
        try {
            DocumentBuilderFactory dbf = DocumentBuilderFactory.newInstance();
            dbf.setNamespaceAware(true);
            // Harden against XXE.
            dbf.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            dbf.setFeature("http://xml.org/sax/features/external-general-entities", false);
            dbf.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
            dbf.setExpandEntityReferences(false);
            DocumentBuilder builder = dbf.newDocumentBuilder();
            Document doc = builder.parse(new ByteArrayInputStream(xml));
            markIdAttributes(doc.getDocumentElement());
            return doc;
        } catch (Exception e) {
            throw new RuntimeException("Failed to parse SAML response: " + e.getMessage(), e);
        }
    }

    /** Register {@code ID} attributes so signature references (#id) resolve. */
    private void markIdAttributes(Element element) {
        if (element.hasAttribute("ID")) {
            element.setIdAttribute("ID", true);
        }
        NodeList children = element.getChildNodes();
        for (int i = 0; i < children.getLength(); i++) {
            Node child = children.item(i);
            if (child instanceof Element el) {
                markIdAttributes(el);
            }
        }
    }

    void verifySignature(Document doc, SamlProvider saml) {
        if (StringUtils.isBlank(saml.getX509Certificate())) {
            throw new RuntimeException("SAML provider has no X.509 certificate configured; cannot verify signature");
        }
        NodeList signatures = doc.getElementsByTagNameNS(DSIG_NS, "Signature");
        if (signatures.getLength() == 0) {
            throw new RuntimeException("SAML response is not signed");
        }
        try {
            PublicKey publicKey = loadCertificate(saml.getX509Certificate()).getPublicKey();
            XMLSignatureFactory factory = XMLSignatureFactory.getInstance("DOM");
            // Validate the first signature (typically on the Response or the Assertion).
            DOMValidateContext ctx = new DOMValidateContext(publicKey, signatures.item(0));
            XMLSignature signature = factory.unmarshalXMLSignature(ctx);
            if (!signature.validate(ctx)) {
                throw new RuntimeException("SAML signature validation failed");
            }
        } catch (RuntimeException e) {
            throw e;
        } catch (Exception e) {
            throw new RuntimeException("SAML signature verification error: " + e.getMessage(), e);
        }
    }

    private X509Certificate loadCertificate(String pem) throws Exception {
        String base64 = pem.replaceAll("-----BEGIN CERTIFICATE-----", "")
                .replaceAll("-----END CERTIFICATE-----", "")
                .replaceAll("\\s", "");
        byte[] der = Base64.getDecoder().decode(base64);
        CertificateFactory cf = CertificateFactory.getInstance("X.509");
        return (X509Certificate) cf.generateCertificate(new ByteArrayInputStream(der));
    }

    void validateConditions(Element assertion) {
        Element conditions = firstElement(assertion, SAML_NS, "Conditions");
        if (conditions == null) {
            return;
        }
        Instant now = Instant.now();
        String notBefore = conditions.getAttribute("NotBefore");
        String notOnOrAfter = conditions.getAttribute("NotOnOrAfter");
        try {
            if (StringUtils.isNotBlank(notBefore) && now.isBefore(Instant.parse(notBefore).minusSeconds(60))) {
                throw new RuntimeException("SAML assertion not yet valid");
            }
            if (StringUtils.isNotBlank(notOnOrAfter) && now.isAfter(Instant.parse(notOnOrAfter).plusSeconds(60))) {
                throw new RuntimeException("SAML assertion has expired");
            }
        } catch (java.time.format.DateTimeParseException ignored) {
            // Be lenient on non-ISO timestamps from some IdPs.
        }
        // Audience must match our SP entityId when present.
        NodeList auds = assertion.getElementsByTagNameNS(SAML_NS, "Audience");
        if (auds.getLength() > 0) {
            boolean match = false;
            for (int i = 0; i < auds.getLength(); i++) {
                if (spEntityId().equals(auds.item(i).getTextContent().trim())) {
                    match = true;
                    break;
                }
            }
            if (!match) {
                throw new RuntimeException("SAML assertion audience mismatch");
            }
        }
    }

    Map<String, Object> extractAttributes(Element assertion) {
        Map<String, Object> attrs = new HashMap<>();
        NodeList attrNodes = assertion.getElementsByTagNameNS(SAML_NS, "Attribute");
        for (int i = 0; i < attrNodes.getLength(); i++) {
            Element attr = (Element) attrNodes.item(i);
            String name = attr.getAttribute("Name");
            String friendly = attr.getAttribute("FriendlyName");
            NodeList values = attr.getElementsByTagNameNS(SAML_NS, "AttributeValue");
            if (values.getLength() > 0) {
                String value = values.item(0).getTextContent().trim();
                if (StringUtils.isNotBlank(name)) attrs.put(name, value);
                if (StringUtils.isNotBlank(friendly)) attrs.put(friendly, value);
            }
        }
        return attrs;
    }

    private String resolveEmail(Element assertion, Map<String, Object> attributes, SamlProvider saml) {
        // Custom mapping wins if configured.
        if (saml.getAttributeMapping() != null) {
            Object emailKey = saml.getAttributeMapping().get("email");
            if (emailKey != null && attributes.get(emailKey.toString()) != null) {
                return attributes.get(emailKey.toString()).toString();
            }
        }
        String email = stringAttr(attributes, "email", "mail",
                "urn:oid:0.9.2342.19200300.100.1.3",
                "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress");
        if (StringUtils.isNotBlank(email)) {
            return email;
        }
        // Fall back to the NameID when it is an email address.
        String nameId = textOf(firstElement(assertion, SAML_NS, "NameID"));
        return nameId != null && nameId.contains("@") ? nameId : null;
    }

    private String stringAttr(Map<String, Object> attrs, String... keys) {
        for (String key : keys) {
            Object v = attrs.get(key);
            if (v != null && StringUtils.isNotBlank(v.toString())) {
                return v.toString();
            }
        }
        return null;
    }

    private Element firstElement(Node scope, String ns, String local) {
        NodeList list = scope instanceof Document d
                ? d.getElementsByTagNameNS(ns, local)
                : ((Element) scope).getElementsByTagNameNS(ns, local);
        return list.getLength() > 0 ? (Element) list.item(0) : null;
    }

    private String textOf(Element el) {
        return el != null ? el.getTextContent().trim() : null;
    }

    private String spEntityId() {
        return domain() + "/auth/v1/sso/saml/metadata";
    }

    private String acsUrl() {
        return domain() + "/auth/v1/sso/saml/acs";
    }

    private String domain() {
        String appCode = MultiTenancyContext.getAppCode();
        return authConfig.getApp().getDomain(appCode);
    }

    private String urlEncode(String value) {
        return java.net.URLEncoder.encode(value, StandardCharsets.UTF_8);
    }
}
