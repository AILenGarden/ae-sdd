package com.example.loan.client;

import com.example.loan.domain.CreditReport;
import com.example.loan.exception.CreditBureauUnavailableException;

public interface CreditBureauClient {
    CreditReport fetchReport(String applicantId) throws CreditBureauUnavailableException;
}
