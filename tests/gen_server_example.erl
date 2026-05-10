-module(gen_server_example).
-behaviour(gen_server).

-export([start_link/0, add/2, get_count/0, stop/0]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2, code_change/3]).

-record(state, {count = 0}).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, [], []).

add(From, Value) ->
    gen_server:call({From, ?MODULE}, {add, Value}).

get_count() ->
    gen_server:call(?MODULE, get_count).

stop() ->
    gen_server:cast(?MODULE, stop).

init([]) ->
    {ok, #state{}}.

handle_call({add, Value}, _From, State) ->
    NewCount = State#state.count + Value,
    {reply, NewCount, State#state{count = NewCount}};
handle_call(get_count, _From, State) ->
    {reply, State#state.count, State}.

handle_cast(stop, State) ->
    {stop, normal, State}.

handle_info(_Info, State) ->
    {noreply, State}.

terminate(_Reason, _State) ->
    ok.

code_change(_OldVsn, State, _Extra) ->
    {ok, State}.
