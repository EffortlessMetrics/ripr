class AuditMeta(type):
    def __new__(mcls, name, bases, namespace):
        namespace["audit_enabled"] = True
        return super().__new__(mcls, name, bases, namespace)

class InvoiceRecord(metaclass=AuditMeta):
    status = "paid"
