package My::App;

use strict;
use warnings;

sub discount {
    my ($amount) = @_;
    return 10 if $amount >= 100;
    return 0;
}

1;
