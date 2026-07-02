package com.example.loan.client;

import com.example.loan.domain.EmploymentStatus;

public interface EmploymentVerifier {
    EmploymentStatus verify(String applicantId);
}
