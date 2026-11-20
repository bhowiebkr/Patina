// Stops a C++ exception of the kernel before it gets to the Rust code (solid.rs `guard`), so that
// a node gets an error and the program does not stop.

#include <memory>
#include <new>

#include "impl.h"

using manifold::Manifold;

// A C++ function type, so that /EHsc does not think that the call cannot throw.
using Run = void (*)(void*);

extern "C" int patina_guard(Run run, void* data) {
  try {
    run(data);
    return 0;
  } catch (const std::bad_alloc&) {
    return static_cast<int>(Manifold::Error::ResultTooLarge);
  } catch (...) {
    return static_cast<int>(Manifold::Error::InvalidConstruction);
  }
}

// Constructs an empty Manifold with the error `status` in `mem`, as Manifold::PropagateStatus.
extern "C" Manifold* patina_manifold_failed(void* mem, int status) {
  auto impl = std::make_shared<Manifold::Impl>();
  impl->status_ = static_cast<Manifold::Error>(status);
  return new (mem) Manifold(Manifold::FromImpl(impl));
}
