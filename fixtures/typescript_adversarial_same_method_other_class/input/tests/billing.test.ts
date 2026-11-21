import { PaymentProcessor } from '../src/billing';

test('validates an eight-character card number', () => {
  const proc = new PaymentProcessor();
  expect(proc.validate("card1234")).toBe(true);
});
