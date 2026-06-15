export function processData(input: string): string {
    if (input.length > 0) {
        return input.trim().toLowerCase();
    }
    return '';
}
