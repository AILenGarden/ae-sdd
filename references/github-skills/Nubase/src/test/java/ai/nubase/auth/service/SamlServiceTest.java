package ai.nubase.auth.service;

import ai.nubase.auth.entity.SamlProvider;
import ai.nubase.common.config.AuthConfig;
import ai.nubase.common.context.MultiTenancyContext;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.w3c.dom.Document;
import org.w3c.dom.Element;

import javax.xml.crypto.dsig.CanonicalizationMethod;
import javax.xml.crypto.dsig.DigestMethod;
import javax.xml.crypto.dsig.Reference;
import javax.xml.crypto.dsig.SignedInfo;
import javax.xml.crypto.dsig.Transform;
import javax.xml.crypto.dsig.XMLSignature;
import javax.xml.crypto.dsig.XMLSignatureFactory;
import javax.xml.crypto.dsig.dom.DOMSignContext;
import javax.xml.crypto.dsig.spec.C14NMethodParameterSpec;
import javax.xml.crypto.dsig.spec.TransformParameterSpec;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.transform.Transformer;
import javax.xml.transform.TransformerFactory;
import javax.xml.transform.dom.DOMSource;
import javax.xml.transform.stream.StreamResult;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.security.KeyFactory;
import java.security.PrivateKey;
import java.security.spec.PKCS8EncodedKeySpec;
import java.time.Instant;
import java.util.Base64;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatCode;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * Unit tests for {@link SamlService}'s security-critical pieces: XML signature verification
 * against the configured IdP cert, assertion Conditions validation, and attribute extraction.
 * Uses a fixed throwaway RSA key/cert and signs assertions in-memory (no DB / network).
 */
@DisplayName("SamlService (assertion verification)")
class SamlServiceTest {

    private static final String SAML_NS = "urn:oasis:names:tc:SAML:2.0:assertion";

    // Throwaway test keypair (PKCS#8) + matching self-signed X.509 cert.
    private static final String KEY_B64 =
            "MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDtjsXQE/hGHEuhaAnbF3Ij1CIr88LCJVm0NZtijwzIJ/cXB6Gn1X6FrpcLGGBcha6G64PNwD12slGnlrCQKo2ZrW2cbDaMXAUT+al9IK6YNRikZ7B2CU+tuKjWAWINux87VRiv4Tk7G+nTPZhbecZQnUqQzo1mu+1FP0z7mLQ7xpmVgMMbJbWsPWGgWAEF2Khl5LjSf3gXDS3svG8qoX76gN/5sQuSIi+Gwtxm7dP3WHONjItfTnmiKizPmo54KjO6sMKZDcbbiOmjyHcsdRc9Ge+1RgcwihrTa4zONLBiMbcQBPb6EYVVsEtFMSOIz9B18vjIZtHCLWPp6kRVCOyfAgMBAAECggEABhoyS6ePq6Fjcfh9tFKUXIZBAHcJDDqwZLgxzyTUk7mvF7ja9rg9wUpcV+0e4JGQi6e2IY1sJsXq4g61Z2aob4esdqWy+wUMHli9+VhjE8bdZK/DMS0JDnHdaG3tqmOhqEEF2tLCyRls9r+wz7y/kuePeEvOBxMRw8MeA6KHErf5IqHcqqgsvNFDm/LNxTnR7b7lyrS4Eti2hjz8HTM9RBNwnp6+/yaIPUJqL6NNNoobjlrnC1Gt+q6IrdbMTbUOj+EKDdcCz+ECrOcMeZrsFRl6LkITG9jr7TN55WoYkPsCoU9AE3jdRrY5iiCxYBiLVzxdDta4OLoyn1jtQgDpEQKBgQD8mfzeMhFDe2oxsAHsj4U98adVTgyhkvtTC0u8m/ccfjNYeSEpKLRXJfKFuTwClypVI3wG6/FdTKuDue85zsYVVLXUNacOdmp3rUyUVbl+8c+0Ud1Zre0v8Tt9jwlJlKyBzKgW0/TM0ADV72OP0wE1D9nPWSWWOa3NHHFcxB3mrwKBgQDwwPiO1ts1vxSpe1YL8S4Pgnv1GmHXHTsVgj8lpEftpwBnLe+D1/mvWkBrDA1iFsKrw3wKw//tECbJ37ChmvdaF+E+FplVpWSdUHjJkp+poEjllp2dyI7PV14yPPc/Gjmdh89SPTwpCcDBY8HxkN4gkH49ZngK4XfbTPh4avPVEQKBgQClMWMgQGfv1McHBY7MkLNXZjDDZc523/OCRJHcH7dEJ/gWNOkNzLPbhlKLRy9Klmc11IXo1OY82MYV2EPtbx81lfdvd0Lv/1rzNx8spr8vmJ33083JNyg1QTJhk2hEeXkzTY7jluuyAZl6TxqyVRCmDd6obilZjBDwYVh1jT/suwKBgQCtBzNcjWR8qzjxWbgM2yhscy4diPl6fgKhwbsalYgwcA7lGOmECyvi7+1OQho4Pf1pLxSuNBFyUmJeQgsTCmntcS4rzlgjarv2KAi3bk6bvZvGjcn3xVWGBNepKZHU40c3RY0mIOZk5CKJmuWfdKuAIfjorgVmZIccsKP+/3cA0QKBgEI/zksfRD4oyUJTx3g0q9xf6wYzmNQTulbJr45mHR4VB/kwf+4kr0sdz3ByMi12eoDxP7+oP5iKtf2S1mm/62CHCRENsk3nq/gYtdamLqJHlRq2ZGtYmh0KHBjznW6zV/98/exovye+yCNGa/WAFox4j3G1VSjFTRAGqj664xb0";

    private static final String CERT_B64 =
            "MIIDBzCCAe+gAwIBAgIUCl0XolqUFCZ/b9cCgfYin3l1r5owDQYJKoZIhvcNAQELBQAwEzERMA8GA1UEAwwIdGVzdC1pZHAwHhcNMjYwNTI5MTQzMjQ1WhcNMzYwNTI2MTQzMjQ1WjATMREwDwYDVQQDDAh0ZXN0LWlkcDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAO2OxdAT+EYcS6FoCdsXciPUIivzwsIlWbQ1m2KPDMgn9xcHoafVfoWulwsYYFyFrobrg83APXayUaeWsJAqjZmtbZxsNoxcBRP5qX0grpg1GKRnsHYJT624qNYBYg27HztVGK/hOTsb6dM9mFt5xlCdSpDOjWa77UU/TPuYtDvGmZWAwxsltaw9YaBYAQXYqGXkuNJ/eBcNLey8byqhfvqA3/mxC5IiL4bC3Gbt0/dYc42Mi19OeaIqLM+ajngqM7qwwpkNxtuI6aPIdyx1Fz0Z77VGBzCKGtNrjM40sGIxtxAE9voRhVWwS0UxI4jP0HXy+Mhm0cItY+nqRFUI7J8CAwEAAaNTMFEwHQYDVR0OBBYEFAXXCwxiYCwjD5QJAQctkt8+hjxQMB8GA1UdIwQYMBaAFAXXCwxiYCwjD5QJAQctkt8+hjxQMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZIhvcNAQELBQADggEBAFVNOO5yHZIS9qRFsvChXZKSx8KlaaBTy1Jkr2UPw94hXEeYG34poqViUj9mkcn3I6bOeh3QUgATYbDdd0FndIJFweREOjsUb4SRslah3cruToDQlvVm3Q9Cl4Ust6EqHKqQ+5z2J9wBBv5XL0y3l8Gb3jpfC2Inp7L9yoOjJIZ9BRnpaPZQkYuobsmlAk54hIardsM79k5IzM7jHWWpVK2CqgceieUKvb55kiIGApzkGknwu918tutQoWTrOePjIPHPxw59MkyQDOhbOQT6G63jvLHAx/GRYV2mNjbsPbQlV/96VYDkA1CePPSFAnYORJj5u364n3eDPT1KOS4V3tE=";

    private SamlService svc;
    private PrivateKey privateKey;

    @BeforeEach
    void setUp() throws Exception {
        svc = new SamlService(null, null, null, null, null, null, null, null, null, null, new AuthConfig());
        privateKey = KeyFactory.getInstance("RSA")
                .generatePrivate(new PKCS8EncodedKeySpec(Base64.getDecoder().decode(KEY_B64)));
        MultiTenancyContext.setContext(MultiTenancyContext.ContextData.builder()
                .appCode("demo").schemaName("public").jwtSecret("s").build());
    }

    @AfterEach
    void clear() {
        MultiTenancyContext.clear();
    }

    private String spEntityId() {
        return "http://demo.localhost:9999/auth/v1/sso/saml/metadata";
    }

    private SamlProvider providerWithCert(String certBody) {
        return SamlProvider.builder().entityId("https://idp.example.com").x509Certificate(certBody).build();
    }

    // ---------------------------------------------------------------- signature verification

    @Test
    @DisplayName("accepts a correctly-signed assertion")
    void validSignature() throws Exception {
        byte[] signed = signResponse(responseXml(spEntityId(), validConditions(), "alice@example.com"));
        Document doc = svc.parse(signed);
        assertThatCode(() -> svc.verifySignature(doc, providerWithCert(CERT_B64))).doesNotThrowAnyException();
    }

    @Test
    @DisplayName("rejects a tampered assertion (digest mismatch)")
    void tamperedSignature() throws Exception {
        byte[] signed = signResponse(responseXml(spEntityId(), validConditions(), "alice@example.com"));
        // flip the email after signing → breaks the reference digest
        String tampered = new String(signed, StandardCharsets.UTF_8).replace("alice@example.com", "evil@example.com");
        Document doc = svc.parse(tampered.getBytes(StandardCharsets.UTF_8));
        assertThatThrownBy(() -> svc.verifySignature(doc, providerWithCert(CERT_B64)))
                .isInstanceOf(RuntimeException.class);
    }

    @Test
    @DisplayName("rejects an unsigned response")
    void unsigned() throws Exception {
        Document doc = svc.parse(responseXml(spEntityId(), validConditions(), "alice@example.com")
                .getBytes(StandardCharsets.UTF_8));
        assertThatThrownBy(() -> svc.verifySignature(doc, providerWithCert(CERT_B64)))
                .isInstanceOf(RuntimeException.class)
                .hasMessageContaining("not signed");
    }

    @Test
    @DisplayName("rejects when no IdP certificate is configured")
    void noCert() throws Exception {
        byte[] signed = signResponse(responseXml(spEntityId(), validConditions(), "alice@example.com"));
        Document doc = svc.parse(signed);
        assertThatThrownBy(() -> svc.verifySignature(doc, providerWithCert(null)))
                .isInstanceOf(RuntimeException.class)
                .hasMessageContaining("certificate");
    }

    // ---------------------------------------------------------------- conditions + attributes

    @Test
    @DisplayName("validateConditions rejects an expired assertion and an audience mismatch")
    void conditions() throws Exception {
        Element okAssertion = assertionOf(responseXml(spEntityId(), validConditions(), "a@b.com"));
        assertThatCode(() -> svc.validateConditions(okAssertion)).doesNotThrowAnyException();

        String expired = "NotBefore=\"" + Instant.now().minusSeconds(600) + "\" NotOnOrAfter=\""
                + Instant.now().minusSeconds(300) + "\"";
        Element expiredAssertion = assertionOf(responseXml(spEntityId(), expired, "a@b.com"));
        assertThatThrownBy(() -> svc.validateConditions(expiredAssertion))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("expired");

        Element wrongAud = assertionOf(responseXml("https://wrong-sp", validConditions(), "a@b.com"));
        assertThatThrownBy(() -> svc.validateConditions(wrongAud))
                .isInstanceOf(RuntimeException.class).hasMessageContaining("audience");
    }

    @Test
    @DisplayName("extractAttributes reads named attributes")
    void attributes() throws Exception {
        Element assertion = assertionOf(responseXml(spEntityId(), validConditions(), "carol@corp.com"));
        Map<String, Object> attrs = svc.extractAttributes(assertion);
        assertThat(attrs.get("email")).isEqualTo("carol@corp.com");
    }

    // ---------------------------------------------------------------- helpers

    private String validConditions() {
        return "NotBefore=\"" + Instant.now().minusSeconds(300) + "\" NotOnOrAfter=\""
                + Instant.now().plusSeconds(300) + "\"";
    }

    private String responseXml(String audience, String conditionTimes, String email) {
        return "<samlp:Response xmlns:samlp=\"urn:oasis:names:tc:SAML:2.0:protocol\""
                + " xmlns:saml=\"" + SAML_NS + "\" ID=\"_resp1\" Version=\"2.0\" InResponseTo=\"_req1\">"
                + "<saml:Issuer>https://idp.example.com</saml:Issuer>"
                + "<saml:Assertion ID=\"_assert1\" Version=\"2.0\" IssueInstant=\"" + Instant.now() + "\">"
                + "<saml:Issuer>https://idp.example.com</saml:Issuer>"
                + "<saml:Subject><saml:NameID Format=\"urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress\">"
                + email + "</saml:NameID></saml:Subject>"
                + "<saml:Conditions " + conditionTimes + ">"
                + "<saml:AudienceRestriction><saml:Audience>" + audience + "</saml:Audience></saml:AudienceRestriction>"
                + "</saml:Conditions>"
                + "<saml:AttributeStatement>"
                + "<saml:Attribute Name=\"email\"><saml:AttributeValue>" + email + "</saml:AttributeValue></saml:Attribute>"
                + "</saml:AttributeStatement>"
                + "</saml:Assertion></samlp:Response>";
    }

    private Element assertionOf(String xml) throws Exception {
        DocumentBuilderFactory dbf = DocumentBuilderFactory.newInstance();
        dbf.setNamespaceAware(true);
        Document doc = dbf.newDocumentBuilder().parse(new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8)));
        return (Element) doc.getElementsByTagNameNS(SAML_NS, "Assertion").item(0);
    }

    /** Sign the Assertion (enveloped, RSA-SHA256) and return the serialized signed XML. */
    private byte[] signResponse(String xml) throws Exception {
        DocumentBuilderFactory dbf = DocumentBuilderFactory.newInstance();
        dbf.setNamespaceAware(true);
        Document doc = dbf.newDocumentBuilder().parse(new ByteArrayInputStream(xml.getBytes(StandardCharsets.UTF_8)));
        Element assertion = (Element) doc.getElementsByTagNameNS(SAML_NS, "Assertion").item(0);
        assertion.setIdAttribute("ID", true);

        XMLSignatureFactory fac = XMLSignatureFactory.getInstance("DOM");
        Reference ref = fac.newReference(
                "#_assert1",
                fac.newDigestMethod(DigestMethod.SHA256, null),
                List.of(fac.newTransform(Transform.ENVELOPED, (TransformParameterSpec) null)),
                null, null);
        SignedInfo si = fac.newSignedInfo(
                fac.newCanonicalizationMethod(CanonicalizationMethod.INCLUSIVE, (C14NMethodParameterSpec) null),
                fac.newSignatureMethod("http://www.w3.org/2001/04/xmldsig-more#rsa-sha256", null),
                List.of(ref));

        DOMSignContext dsc = new DOMSignContext(privateKey, assertion);
        Element issuer = (Element) assertion.getElementsByTagNameNS(SAML_NS, "Issuer").item(0);
        if (issuer != null && issuer.getNextSibling() != null) {
            dsc.setNextSibling(issuer.getNextSibling());
        }
        XMLSignature signature = fac.newXMLSignature(si, null);
        signature.sign(dsc);

        Transformer t = TransformerFactory.newInstance().newTransformer();
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        t.transform(new DOMSource(doc), new StreamResult(bos));
        return bos.toByteArray();
    }
}
