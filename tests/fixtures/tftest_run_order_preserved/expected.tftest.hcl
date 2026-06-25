run "z_setup" {
  command = plan
}

run "a_apply" {
  command = apply
}

run "m_verify" {
  command = plan

  assert {
    condition     = output.id != ""
    error_message = "missing id"
  }
}
