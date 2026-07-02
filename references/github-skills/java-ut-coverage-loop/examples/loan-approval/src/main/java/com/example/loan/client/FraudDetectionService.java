package com.example.loan.client;

public interface FraudDetectionService {
    boolean isFlagged(String applicantId);
}
