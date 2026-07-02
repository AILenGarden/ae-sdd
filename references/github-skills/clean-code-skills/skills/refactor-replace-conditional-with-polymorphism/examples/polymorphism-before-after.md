# Polymorphism Before/After Examples

## Example 1: Payment Processing

### Before — Type-Based Conditional
```
function processPayment(payment) {
  if (payment.type === 'credit_card') {
    validateCardNumber(payment.cardNumber)
    chargeCard(payment.cardNumber, payment.amount)
    return { status: 'charged', fee: payment.amount * 0.029 }
  } else if (payment.type === 'bank_transfer') {
    validateRoutingNumber(payment.routingNumber)
    initiateBankTransfer(payment.routingNumber, payment.amount)
    return { status: 'pending', fee: 1.50 }
  } else if (payment.type === 'crypto') {
    validateWalletAddress(payment.walletAddress)
    sendCryptoPayment(payment.walletAddress, payment.amount)
    return { status: 'broadcast', fee: 0 }
  }
}
```

**Problems:** Adding a new payment type means modifying this function. Each branch has different validation, execution, and fee logic tangled together.

### After — Polymorphic Dispatch
```
interface PaymentProcessor {
  validate(payment): void
  execute(payment): void
  calculateFee(amount): number
  resultStatus(): string
}

class CreditCardProcessor implements PaymentProcessor {
  validate(payment) { validateCardNumber(payment.cardNumber) }
  execute(payment) { chargeCard(payment.cardNumber, payment.amount) }
  calculateFee(amount) { return amount * 0.029 }
  resultStatus() { return 'charged' }
}

class BankTransferProcessor implements PaymentProcessor {
  validate(payment) { validateRoutingNumber(payment.routingNumber) }
  execute(payment) { initiateBankTransfer(payment.routingNumber, payment.amount) }
  calculateFee(amount) { return 1.50 }
  resultStatus() { return 'pending' }
}

class CryptoProcessor implements PaymentProcessor {
  validate(payment) { validateWalletAddress(payment.walletAddress) }
  execute(payment) { sendCryptoPayment(payment.walletAddress, payment.amount) }
  calculateFee(amount) { return 0 }
  resultStatus() { return 'broadcast' }
}

// Usage
function processPayment(payment, processor: PaymentProcessor) {
  processor.validate(payment)
  processor.execute(payment)
  return { status: processor.resultStatus(), fee: processor.calculateFee(payment.amount) }
}
```

---

## Example 2: Notification Delivery

### Before
```
function sendNotification(user, message) {
  switch (user.preferredChannel) {
    case 'email':
      emailClient.send(user.email, message.subject, message.body)
      break
    case 'sms':
      smsGateway.send(user.phone, message.body.substring(0, 160))
      break
    case 'push':
      pushService.send(user.deviceToken, message.subject, message.body)
      break
  }
}
```

### After
```
interface NotificationChannel {
  send(user, message): void
}

class EmailChannel implements NotificationChannel {
  send(user, message) {
    emailClient.send(user.email, message.subject, message.body)
  }
}

class SmsChannel implements NotificationChannel {
  send(user, message) {
    smsGateway.send(user.phone, message.body.substring(0, 160))
  }
}

class PushChannel implements NotificationChannel {
  send(user, message) {
    pushService.send(user.deviceToken, message.subject, message.body)
  }
}

// Factory
const channels = { email: new EmailChannel(), sms: new SmsChannel(), push: new PushChannel() }

function sendNotification(user, message) {
  const channel = channels[user.preferredChannel]
  channel.send(user, message)
}
```

---

## When NOT to Use Polymorphism

- **Two simple branches**: `if (isAdmin) { ... } else { ... }` — polymorphism is overkill.
- **Pure data mapping**: use a lookup table or map instead. `const TAX_RATES = { 'US': 0.08, 'UK': 0.20 }`.
- **Temporary conditions**: feature flags or one-off checks that will be removed.
- **Performance-critical paths**: virtual dispatch adds indirection; measure before refactoring.

## Decision Checklist
1. [ ] Does the conditional branch on type, status, or category?
2. [ ] Does the same branching structure appear in more than one method?
3. [ ] Is it likely that new types will be added in the future?
4. [ ] Does each branch contain non-trivial logic (more than a single expression)?

If you answer "yes" to 3 or more, polymorphism is the right move.
