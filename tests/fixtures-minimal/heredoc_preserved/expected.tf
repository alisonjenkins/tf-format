locals {
  z_script = <<-EOT
    deploy step   
    	indented tab line	

    trailing blank above kept
  EOT
  a_name = "demo"
  config = <<EOF
plain heredoc line with spaces    
no indent strip here
EOF
}
