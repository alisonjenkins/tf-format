locals {
  a_name = "demo"

  config = <<EOF
plain heredoc line with spaces    
no indent strip here
EOF

  z_script = <<-EOT
    deploy step   
    	indented tab line	

    trailing blank above kept
  EOT
}
