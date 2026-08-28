export class TokenValidator {
  private valid: Set<string>;
  constructor(valid: string[]) {
    this.valid = new Set(valid);
  }
  validate(token: string): boolean {
    return this.valid.has(token.trim());
  }
}
