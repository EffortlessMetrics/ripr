from src.session import Session
from src.conn import Conn


def test_session_refresh():
    Session().refresh()
    conn = Conn()
    assert conn.status == "closed"
