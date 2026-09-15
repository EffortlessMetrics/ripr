use strict;
use warnings;

use Test::More;

use App::Discount;

is(App::Discount::discount(undef, 5),  0,  'no discount below first threshold');
is(App::Discount::discount(undef, 25), 5,  'mid tier discount');
is(App::Discount::discount(undef, 80), 10, 'top tier discount');

done_testing();
