package App::Discount;

use strict;
use warnings;

sub discount {
    my ($self, $amount) = @_;

    return 0 if $amount < 10;
    return 5  if $amount < 50;
    return 10;
}

1;
