package com.example.loan.domain;

import java.math.BigDecimal;
import java.time.LocalDate;

public record Applicant(
        String id,
        String fullName,
        LocalDate dateOfBirth,
        BigDecimal annualIncome,
        BigDecimal monthlyDebt
) {
    public Applicant {
        if (id == null || id.isBlank()) {
            throw new IllegalArgumentException("id required");
        }
        if (dateOfBirth == null) {
            throw new IllegalArgumentException("dateOfBirth required");
        }
        if (annualIncome == null || annualIncome.signum() < 0) {
            throw new IllegalArgumentException("annualIncome must be non-negative");
        }
        if (monthlyDebt == null || monthlyDebt.signum() < 0) {
            throw new IllegalArgumentException("monthlyDebt must be non-negative");
        }
    }
}
