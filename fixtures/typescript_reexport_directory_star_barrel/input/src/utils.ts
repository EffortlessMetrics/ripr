/** Remove `base` from the start of `input` (unjs/ufo `withoutBase` shape). */
export function withoutBase(input: string, base: string): string {
    if (!input.startsWith(base)) {
        return input;
    }
    const trimmed = input.slice(base.length);
    return trimmed[0] === '/' ? trimmed : '/' + trimmed;
}
