export function validateScore(score: number): boolean {
    if (score >= 60) {
        return true;
    }
    return false;
}
