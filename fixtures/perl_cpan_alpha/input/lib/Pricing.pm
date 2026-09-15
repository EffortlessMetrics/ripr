package Pricing;

use strict;
use warnings;

sub calculate_discount {
    my ($amount) = @_;
    if ($amount >= 100) {
        return $amount * 0.9;
    }
    return $amount;
}

sub dynamic_method {
    my ($self, $method) = @_;
    return $self->$method();
}

1;
