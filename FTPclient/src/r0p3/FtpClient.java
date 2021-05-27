package r0p3;

import java.net.*;
import java.io.*;

public class FtpClient {

	private String address;
	private int port;

	private Socket socket;
	private BufferedReader input;
	private PrintWriter output;

	// private BufferedReader in_keyboad;
	// private String data;
	// private String result;

	
	public FtpClient(String a, int p) {
		this.address = a;
		this.port = p;
	}

	
	public void startConnection() {
		if (this.socket == null) {
			try {
				this.socket = new Socket(this.address, this.port);
				this.input = new BufferedReader(
						new InputStreamReader(this.socket.getInputStream()
				));
				this.output = new PrintWriter(this.socket.getOutputStream(), true);

			} catch (UnknownHostException err) {
				System.err.println(err);
			} catch (IOException err) {
				System.err.println(err);
			}
		} else {
			System.err.println("Socket already connected.");
			System.err.println("Close the connection before connect a new Server.");
		}
	}


	public void sendCommand(String command) {
		output.print(command);
		output.flush();
	}

	public String receiveCommand() {
		try {
			return input.readLine();
		} catch (IOException err) {
			return null;
		}
	}


	public void close() throws IOException {
		System.out.println("Clossing connection with " + this.socket.getRemoteSocketAddress());
		this.socket.close();
	}

}
