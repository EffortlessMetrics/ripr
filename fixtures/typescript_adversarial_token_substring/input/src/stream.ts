export function buffer(chunks: string[]): string {
  return chunks.join("|");
}
export function bufferedStream(chunks: string[]): number {
  return chunks.length;
}
