class Api:
    def post(self, path):
        def decorator(func):
            return func

        return decorator


class Response:
    def __init__(self, status_code, detail):
        self.status_code = status_code
        self.detail = detail


api = Api()


@api.post("/checkout")
def checkout(coupon_expired):
    response = Response(200, "ok")
    if coupon_expired:
        response.status_code = 422
        response.detail = "coupon expired"
    return response
