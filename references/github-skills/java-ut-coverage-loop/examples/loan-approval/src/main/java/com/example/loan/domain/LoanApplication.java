package com.example.loan.domain;

import java.math.BigDecimal;

public record LoanApplication(
        String id,
        Applicant applicant,
        BigDecimal amount,
        int termMonths,
        String purpose
) {
}
