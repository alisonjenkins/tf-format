locals {
  port = 19142 # top-level comment

  commas = {
    p = 1, # comment p
    q = 2, # comment q
  }

  config = {
    port = 19142 # nested comment
  }

  deep = {
    a = {
      b = {
        port = 1 # three levels deep
      }
    }
  }
}
