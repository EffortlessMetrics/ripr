export function formatBadge(count: number): string {
  return count > 99 ? "99+" : String(count);
}
export function Badge({ count }: { count: number }): string {
  return `<span class="badge">${formatBadge(count)}</span>`;
}
