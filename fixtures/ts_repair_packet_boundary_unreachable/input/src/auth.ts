export function login(user: string): string {
  if (user.length > 3) {
    return session(user);
  }
  throw new Error("too short");
}
