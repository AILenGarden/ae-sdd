package com.example.loan.domain;

import java.time.LocalDate;

public record CreditReport(
        String applicantId,
        int score,
        int openAccounts,
        int delinquencies,
        LocalDate reportedAt
) {
    public CreditReport {
        if (score < 300 || score > 850) {
            throw new IllegalArgumentException("FICO score must be 300..850");
        }
    }

    public boolean hasRecentDelinquency() {
        return delinquencies > 0;
    }
}
