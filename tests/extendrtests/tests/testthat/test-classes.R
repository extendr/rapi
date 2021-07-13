test_that("Exported class works", {
  x <- MyClass$new()
  # Using getter
  expect_equal(x$a, 0L)
  # Using setter
  x$a <- 10L
  expect_equal(x$a, 10L)
  expect_equal(x$me(), x)

  # Directly invoking methods
  expect_visible(x$get_a())
  expect_invisible(x$set_a(5L))

  # Verifying methods and accessors do the same
  expect_equal(x$a, 5L)

})

test_that("Unexported class works", {
  # unexported code works in testthat tests
  x <- MyClassUnexported$new()
  expect_equal(x$a, 22L)
})
