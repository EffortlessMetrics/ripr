from dataclasses import dataclass

@dataclass
class User:
    active: bool

def build_user():
    return User(active=True)
