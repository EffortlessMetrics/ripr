class PriceRule:
    @classmethod
    def from_config(cls, config):
        return cls(config["discount"] + 1)
