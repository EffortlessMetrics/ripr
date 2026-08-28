export class PaymentProcessor {
  validate(card: string): boolean {
    return card.length === 8;
  }
}
