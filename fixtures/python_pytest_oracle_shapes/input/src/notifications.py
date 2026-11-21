import logging

logger = logging.getLogger(__name__)

def warn_coupon(code):
    if code == "expired":
        logger.warning("coupon expired")
        return "blocked"
    return "ok"
