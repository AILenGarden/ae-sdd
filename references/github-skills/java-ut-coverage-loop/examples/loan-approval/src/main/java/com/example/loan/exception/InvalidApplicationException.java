package com.example.loan.exception;

public class InvalidApplicationException extends RuntimeException {
    public InvalidApplicationException(String message) {
        super(message);
    }
}
