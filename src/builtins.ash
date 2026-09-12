# We define type functions as named functions so that they'll have
# proper names with reflection. We store these in distinctly named
# `<type>s_<func>` variables to pass them from Ash to the actual
# `TypeFunctions` values, in order to prevent shadowing.

# TODO Replace this with the final `object_match` implementation - we
# use the current implementation to test the mechanism for using Ash to
# define built-in functions.
fn object_match(that) {
    print("TODO")
}

objects_match = object_match;
