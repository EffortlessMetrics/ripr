class Table:
    def __init__(self):
        self.columns = [1, 2]

    def __rich_console__(self, console, options):
        return self._calculate_column_widths()

    def _calculate_column_widths(self):
        return self._get_padding_width(0)

    def _get_padding_width(self, column_index):
        if column_index >= len(self.columns) - 1:
            return 0
        return 1
