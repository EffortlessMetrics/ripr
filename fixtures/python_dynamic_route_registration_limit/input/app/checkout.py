class Api:
    def post(self, path):
        def decorator(func):
            return func

        return decorator


def route_path():
    return "/checkout"


api = Api()


@api.post(route_path())
def checkout(payload):
    if payload.get("expired"):
        return {"detail": "coupon expired"}
    return {"detail": "ok"}
