package com.example.loan.domain;

import java.math.BigDecimal;

public record LoanDecision(
        DecisionStatus status,
        DecisionReason reason,
        BigDecimal approvedAmount,
        BigDecimal annualInterestRate
) {
    public static LoanDecision approved(DecisionReason reason, BigDecimal amount, BigDecimal rate) {
        return new LoanDecision(DecisionStatus.APPROVED, reason, amount, rate);
    }

    public static LoanDecision declined(DecisionReason reason) {
        return new LoanDecision(DecisionStatus.DECLINED, reason, null, null);
    }

    public static LoanDecision referred(DecisionReason reason) {
        return new LoanDecision(DecisionStatus.REFERRED, reason, null, null);
    }

    public boolean isApproved() {
        return status == DecisionStatus.APPROVED;
    }
}
