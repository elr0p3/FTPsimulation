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
		result = sc.interpretStatusCode(passwd, rcv);

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
			ft.getPortPasive(rcv);
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
				result = sc.interpretStatusCode(command, rcv);

				if (result == StatusCode.EXIT) {
					fc.close();
					ft.closeConnection();
					break;

				} else if (result == StatusCode.LIST) {
					String data = ft.listFile();
					System.out.println(data);
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);

				} else if (result == StatusCode.DOWN) {
					byte[] data = ft.downloadFile();
					String filename = cmd.inputData("local");
					ft.storeDataFile(data, filename);
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);

				} else if (result == StatusCode.UP) {
					// command.split(" ")[1].trim()
					String filename = cmd.inputData("local");
					byte[] data = ft.getDataFile(filename);
					ft.uploadFile(data);
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);

				} else if (result == StatusCode.MOVE) {
					String name = cmd.inputData("future");
					command = cmd.renameTo(name);
					fc.sendCommand(command);
					rcv = fc.receiveCommand();
					System.out.println("srv: " + rcv);
				} 

			// ft.closeConnection();
			// Mode selected in the beginning
			fc.sendCommand(mode);
			rcv = fc.receiveCommand();
			if (mode.startsWith(Command.PASIVE)) {
				ft.getPortPasive(rcv);
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
		
		ft.closeConnection();
	}

}
