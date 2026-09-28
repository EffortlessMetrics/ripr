use strict;
use warnings;

use Test::More;
use My::App;

is(My::App::discount(100), 10, 'discount threshold');

done_testing();
