package com.example.loan.repo;

import com.example.loan.domain.LoanApplication;
import com.example.loan.domain.LoanDecision;

public interface LoanRepository {
    void save(LoanApplication application, LoanDecision decision);
}
