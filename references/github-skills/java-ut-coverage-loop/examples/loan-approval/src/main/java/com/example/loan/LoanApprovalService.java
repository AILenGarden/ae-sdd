package com.example.loan;

import com.example.loan.client.CreditBureauClient;
import com.example.loan.client.EmploymentVerifier;
import com.example.loan.client.FraudDetectionService;
import com.example.loan.domain.Applicant;
import com.example.loan.domain.CreditReport;
import com.example.loan.domain.DecisionReason;
import com.example.loan.domain.EmploymentStatus;
import com.example.loan.domain.LoanApplication;
import com.example.loan.domain.LoanDecision;
import com.example.loan.exception.CreditBureauUnavailableException;
import com.example.loan.exception.InvalidApplicationException;
import com.example.loan.repo.LoanRepository;

import java.math.BigDecimal;
import java.math.RoundingMode;
import java.time.Clock;
import java.time.LocalDate;
import java.time.Period;

/**
 * Underwrites consumer loan applications by combining identity, employment,
 * income, debt, and credit-bureau signals. Pure business logic; all I/O is
 * delegated to injected collaborators so the class is unit-testable in isolation.
 */
public class LoanApprovalService {

    private static final BigDecimal MIN_ANNUAL_INCOME = new BigDecimal("30000");
    private static final BigDecimal MAX_DTI = new BigDecimal("0.43");
    private static final BigDecimal PRIME_INCOME_MULTIPLE = new BigDecimal("5");
    private static final BigDecimal STANDARD_INCOME_MULTIPLE = new BigDecimal("3");
    private static final int MIN_AGE = 18;
    private static final int MAX_AGE = 75;
    private static final int MIN_CREDIT_SCORE = 600;
    private static final int PRIME_CREDIT_SCORE = 750;

    private final CreditBureauClient creditBureau;
    private final FraudDetectionService fraudDetection;
    private final EmploymentVerifier employmentVerifier;
    private final LoanRepository loanRepository;
    private final Clock clock;

    public LoanApprovalService(
            CreditBureauClient creditBureau,
            FraudDetectionService fraudDetection,
            EmploymentVerifier employmentVerifier,
            LoanRepository loanRepository,
            Clock clock) {
        this.creditBureau = creditBureau;
        this.fraudDetection = fraudDetection;
        this.employmentVerifier = employmentVerifier;
        this.loanRepository = loanRepository;
        this.clock = clock;
    }

    /**
     * Run the full underwriting cascade. Returns APPROVED, DECLINED, or REFERRED
     * with a reason code. Persists APPROVED decisions via the repository.
     */
    public LoanDecision evaluate(LoanApplication application) {
        if (application == null) {
            throw new IllegalArgumentException("application cannot be null");
        }
        if (application.applicant() == null || application.amount() == null) {
            throw new InvalidApplicationException("missing applicant or amount");
        }
        if (application.amount().signum() <= 0) {
            throw new InvalidApplicationException("amount must be positive");
        }
        if (application.termMonths() < 6 || application.termMonths() > 120) {
            throw new InvalidApplicationException("term must be 6..120 months");
        }

        Applicant applicant = application.applicant();
        int age = computeAge(applicant.dateOfBirth());

        if (age < MIN_AGE) {
            return LoanDecision.declined(DecisionReason.UNDERAGE);
        }
        if (age > MAX_AGE) {
            return LoanDecision.declined(DecisionReason.OVER_AGE_LIMIT);
        }

        if (fraudDetection.isFlagged(applicant.id())) {
            return LoanDecision.declined(DecisionReason.FRAUD_FLAGGED);
        }

        EmploymentStatus employment;
        try {
            employment = employmentVerifier.verify(applicant.id());
        } catch (RuntimeException e) {
            return LoanDecision.referred(DecisionReason.EMPLOYMENT_VERIFICATION_FAILED);
        }
        if (employment == EmploymentStatus.UNEMPLOYED) {
            return LoanDecision.declined(DecisionReason.UNEMPLOYED);
        }

        if (applicant.annualIncome().compareTo(MIN_ANNUAL_INCOME) < 0) {
            return LoanDecision.declined(DecisionReason.INSUFFICIENT_INCOME);
        }

        BigDecimal dti = computeDti(applicant.monthlyDebt(), applicant.annualIncome());
        if (dti.compareTo(MAX_DTI) > 0) {
            return LoanDecision.declined(DecisionReason.HIGH_DTI);
        }

        CreditReport report;
        try {
            report = creditBureau.fetchReport(applicant.id());
        } catch (CreditBureauUnavailableException e) {
            return LoanDecision.referred(DecisionReason.CREDIT_BUREAU_UNAVAILABLE);
        }
        int creditScore = report.score();
        if (creditScore < MIN_CREDIT_SCORE) {
            return LoanDecision.declined(DecisionReason.LOW_CREDIT_SCORE);
        }

        BigDecimal maxAmount = computeMaxAmount(applicant.annualIncome(), creditScore);
        if (application.amount().compareTo(maxAmount) > 0) {
            return LoanDecision.declined(DecisionReason.AMOUNT_EXCEEDS_LIMIT);
        }

        DecisionReason approvalReason = creditScore >= PRIME_CREDIT_SCORE
                ? DecisionReason.APPROVED_PRIME
                : DecisionReason.APPROVED_STANDARD;
        BigDecimal rate = computeInterestRate(creditScore, application.termMonths(), employment);
        LoanDecision decision = LoanDecision.approved(approvalReason, application.amount(), rate);
        loanRepository.save(application, decision);
        return decision;
    }

    /**
     * Lightweight pre-approval check that only consults the credit bureau and
     * income floor; never persists. Returns false on any exception or missing
     * applicant.
     */
    public boolean isPreApproved(Applicant applicant) {
        if (applicant == null) {
            return false;
        }
        BigDecimal floor = MIN_ANNUAL_INCOME.multiply(new BigDecimal("2"));
        if (applicant.annualIncome().compareTo(floor) < 0) {
            return false;
        }
        try {
            CreditReport report = creditBureau.fetchReport(applicant.id());
            return report.score() >= PRIME_CREDIT_SCORE && !report.hasRecentDelinquency();
        } catch (CreditBureauUnavailableException e) {
            return false;
        }
    }

    /**
     * Standard amortization formula. Returns zero-rate fallback when rate is
     * zero. Throws on non-positive principal or term.
     */
    public BigDecimal computeMonthlyPayment(BigDecimal principal, BigDecimal annualRate, int termMonths) {
        if (principal == null || principal.signum() <= 0) {
            throw new IllegalArgumentException("principal must be positive");
        }
        if (termMonths <= 0) {
            throw new IllegalArgumentException("termMonths must be positive");
        }
        if (annualRate == null || annualRate.signum() < 0) {
            throw new IllegalArgumentException("annualRate must be non-negative");
        }
        if (annualRate.signum() == 0) {
            return principal.divide(new BigDecimal(termMonths), 2, RoundingMode.HALF_UP);
        }
        double r = annualRate.doubleValue() / 12.0;
        double n = termMonths;
        double p = principal.doubleValue();
        double payment = p * (r * Math.pow(1 + r, n)) / (Math.pow(1 + r, n) - 1);
        return BigDecimal.valueOf(payment).setScale(2, RoundingMode.HALF_UP);
    }

    private BigDecimal computeMaxAmount(BigDecimal annualIncome, int creditScore) {
        BigDecimal multiple = creditScore >= PRIME_CREDIT_SCORE
                ? PRIME_INCOME_MULTIPLE
                : STANDARD_INCOME_MULTIPLE;
        return annualIncome.multiply(multiple);
    }

    private BigDecimal computeInterestRate(int creditScore, int termMonths, EmploymentStatus employment) {
        BigDecimal base;
        if (creditScore >= PRIME_CREDIT_SCORE) {
            base = new BigDecimal("0.045");
        } else if (creditScore >= 700) {
            base = new BigDecimal("0.060");
        } else if (creditScore >= 650) {
            base = new BigDecimal("0.080");
        } else {
            base = new BigDecimal("0.110");
        }
        if (termMonths > 60) {
            base = base.add(new BigDecimal("0.005"));
        }
        if (employment == EmploymentStatus.SELF_EMPLOYED || employment == EmploymentStatus.CONTRACTOR) {
            base = base.add(new BigDecimal("0.010"));
        }
        return base;
    }

    private BigDecimal computeDti(BigDecimal monthlyDebt, BigDecimal annualIncome) {
        BigDecimal monthlyIncome = annualIncome.divide(new BigDecimal("12"), 2, RoundingMode.HALF_UP);
        if (monthlyIncome.signum() == 0) {
            return BigDecimal.ONE;
        }
        return monthlyDebt.divide(monthlyIncome, 4, RoundingMode.HALF_UP);
    }

    private int computeAge(LocalDate dob) {
        return Period.between(dob, LocalDate.now(clock)).getYears();
    }
}
