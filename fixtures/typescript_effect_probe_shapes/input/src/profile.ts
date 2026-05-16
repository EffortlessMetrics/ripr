type Profile = { status: string };

export function updateProfile(profile: Profile, status: string, audit: { record(value: string): void }): Profile {
    profile.status = status;
    audit.record(status);
    return profile;
}
