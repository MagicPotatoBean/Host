# Host.rs
Allows users to temporarily host their files over LAN or port-forwarded/tunneled to the internet
## Examples
$ host.rs -f ./mydir/myfile.txt myfile.txt -a 0.0.0.0:80
- Use 0.0.0.0 to host on all network interfaces available
- This particular script would need sudo, since the port is less than 1024
