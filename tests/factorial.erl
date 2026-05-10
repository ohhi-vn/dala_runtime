-module(test_factorial).
-export([factorial/1, test/0]).

factorial(0) -> 1;
factorial(N) when N > 0 -> N * factorial(N - 1).

test() ->
    1 = factorial(0),
    1 = factorial(1),
    2 = factorial(2),
    6 = factorial(3),
    24 = factorial(4),
    120 = factorial(5),
    3628800 = factorial(10),
    ok.
