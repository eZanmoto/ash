# We define type functions as named functions so that they'll have
# proper names with reflection. We store these in distinctly named
# `<type>s_<func>` variables to pass them from Ash to the actual
# `TypeFunctions` values, in order to prevent shadowing.

# `object_match(err_obj, shape)` returns `[matched_obj, true]` iff `err_obj` or
# one of its (recursive) `sources` matches `shape`, or `[null, false]`
# otherwise.
#
# An error object matches a shape iff their `code` and `func` properties are
# equal. If an object doesn't match the shape then its sources will be checked
# for equality in a depth-first order. The first object that matches the shape
# will be returned.
fn object_match(that) {
    if that::len() == 0 {
        throw "target error must contain 'code' and/or 'func'"
    }

    stack $:= [this]
    while stack::len() > 0 {
        [err, rest] := stack->pop();
        stack = rest

        [sources, ok] := ? err.sources
        if ok {
            stack += sources
        }

        if err->err_match(that) {
            return [err, true]
        }
    }

    return [null, false]
}

# `pop` returns `[last, rest]`, where `last` is the last element of `stack` and
# `rest` is the list of items before it, or `[]` if `stack` only contains one
# item.
#
# TODO Allow `[a, b..] = xs` when `xs` only contains one item.
fn pop(stack) {
    len := stack::len()
    last := stack[len - 1]

    if len == 1 {
        return [last, []]
    }

    return [last, stack[0:len - 1]]
}

fn err_match(src_err, tgt_err) {
    [_, ok] := ? src_err["func"]
    if !ok {
        throw "source error doesn't contain 'func'"
    }

    for [prop_name, that_prop] in tgt_err {
        if prop_name == "code" {
            tgt_code := that_prop

            [src_code, ok] := ? src_err.code
            if !ok {
                return false
            }

            if src_code != tgt_code {
                return false
            }
        } else if prop_name == "func" {
            tgt_func := that_prop
            # `src_err` should always contain `func`, based on the
            # implementation of the runtime.
            #
            # TODO Consider whether to convert an error in retrieval to a "dev
            # err".
            if src_err["func"] !== tgt_func {
                return false
            }
        } else {
            throw "target error may only contain 'code' and/or 'func'"
        }
    }

    return true
}

objects_match = object_match;
