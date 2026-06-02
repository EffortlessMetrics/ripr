function tracked(_target: unknown, _context: unknown) {}

class Service {
    @tracked
    save(value: string): string {
        return value.trim();
    }
}
