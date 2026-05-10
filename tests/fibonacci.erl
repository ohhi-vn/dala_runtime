-module(test_fibonacci).
-export([fib/1, test/0]).

fib(0) -> 0;
fib(1) -> 1;
fib(N) when N > 1 -> fib(N - 1) + fib(N - 2).

test() ->
    0 = fib(0),
    1 = fib(1),
    1 = fib(2),
    2 = fib(3),
    3 = fib(4),
    5 = fib(5),
    8 = fib(6),
    55 = fib(10),
    ok.
