use strict;
use warnings;
use Test::More;
use Pricing;

# Weak oracle — doesn't pin the exact boundary
ok(calculate_discount(100), 'discount applies');

# Exact oracle — already discriminates the boundary
is(calculate_discount(100), 90, 'discount is 10% at threshold');

done_testing();
