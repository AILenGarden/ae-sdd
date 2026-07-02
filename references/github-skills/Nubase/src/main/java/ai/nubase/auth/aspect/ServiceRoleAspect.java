package ai.nubase.auth.aspect;

import ai.nubase.auth.exception.ForbiddenException;
import ai.nubase.common.context.MultiTenancyContext;
import lombok.extern.slf4j.Slf4j;
import org.aspectj.lang.JoinPoint;
import org.aspectj.lang.annotation.Aspect;
import org.aspectj.lang.annotation.Before;
import org.springframework.stereotype.Component;

/**
 * AOP Aspect to enforce service_role key authentication for admin operations.
 * Intercepts methods/classes annotated with @RequireServiceRole.
 */
@Aspect
@Component
@Slf4j
public class ServiceRoleAspect {

    /**
     * Check if the current request is using service_role key.
     * Throws ForbiddenException if not authorized.
     */
    @Before("@within(ai.nubase.auth.annotation.RequireServiceRole) || " +
            "@annotation(ai.nubase.auth.annotation.RequireServiceRole)")
    public void checkServiceRole(JoinPoint joinPoint) {
        if (!MultiTenancyContext.isServiceRole()) {
            String methodName = joinPoint.getSignature().toShortString();
            log.warn("Unauthorized admin access attempt to {} from non-service-role context", methodName);
            throw new ForbiddenException("Admin operations require service role key");
        }

        log.debug("Service role check passed for {}", joinPoint.getSignature().toShortString());
    }
}
