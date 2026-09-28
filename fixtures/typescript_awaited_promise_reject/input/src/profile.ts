export async function loadProfile(id: string): Promise<string> {
    if (!id) {
        return await Promise.reject(new Error("missing id"));
    }
    return `profile:${id}`;
}

export async function publishProfile(id: string, sink: { publish(id: string): Promise<void> }): Promise<void> {
    if (!id) {
        await Promise.reject(new Error("missing id"));
    }
    await sink.publish(id);
}
