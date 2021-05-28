package r0p3;

import java.io.IOException;

public class Main {

	public static void main(String[] args) throws IOException {

		String command, rcv, mode;
		int result;
		Command cmd = new Command();
		StatusCode sc = new StatusCode();
		String address = args[0];	// localhost
		int port = Integer.parseInt(args[1]);	// 8080

		System.out.println("FTP Client Started!");

		FtpClient fc = new FtpClient(address, port);
		fc.startConnection();
		FileTransfer ft = new FileTransfer(address);

		rcv = fc.receiveCommand();
		System.out.println("srv: " + rcv);


		// Validating user
		String userName = cmd.inputUserName();
		fc.sendCommand(userName);
		rcv = fc.receiveCommand();
		System.out.println("srv: " + rcv);

		String passwd = cmd.inputPasswd();
		fc.sendCommand(passwd);
		rcv = fc.receiveCommand();
		System.out.println("srv: " + rcv);
		result = sc.interpretStatusCode(passwd, rcv, cmd, ft);

		if (result == StatusCode.ERROR) {
			fc.close();
			System.exit(1);
		}


		// Active Pasive mode with server
		mode = cmd.selectMode(ft.getIpPort());
		fc.sendCommand(mode);
		rcv = fc.receiveCommand();
		System.out.println("srv: " + rcv);
		if (mode.startsWith(Command.PASIVE)) {
			ft.setPortPasive(rcv);
			ft.startPasive();
			rcv = fc.receiveCommand();
			System.out.println("srv: " + rcv);
		} else {
			ft.acceptConnection();
		}


		// Command, codes, files exchange
		while (true) {

			command = cmd.inputCommand();
			fc.sendCommand(command);
			rcv = fc.receiveCommand();
			
			if (rcv != null) {
				System.out.println("srv: " + rcv);
				result = sc.interpretStatusCode(command, rcv, cmd, ft);
				System.out.println("MARIKONG - " + result);

				if (result == StatusCode.EXIT) {
					break;

				} else if (result == StatusCode.LIST
						|| result == StatusCode.DOWN
						|| result == StatusCode.UP) {
					
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);

				} else if (result == StatusCode.MOVE) {
					String name = cmd.inputData("future");
					command = cmd.renameTo(name);
					fc.sendCommand(command);
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);
				} 

			// Mode selected in the beginning
			fc.sendCommand(mode);
			rcv = fc.receiveCommand();
			if (mode.startsWith(Command.PASIVE)) {
				ft.setPortPasive(rcv);
				ft.startPasive();
				rcv = fc.receiveCommand();
			} else {
				ft.acceptConnection();
			}


			} else {
				ft.closeConnection();
				fc.close();
				break;
			}
		}
		
		fc.close();
		ft.closeConnection();
	}

}
