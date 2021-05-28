package r0p3;

import java.net.*;
import java.nio.file.Files;
import java.util.ArrayList;
import java.io.*;

public class FileTransfer {

	private String address;
	private int port;

	private String ipPortNumber;
	private ServerSocket sServ;
	private Socket sCon;
	private DataInputStream input;
	private DataOutputStream output;

	public FileTransfer(String address) {
		// https://stackoverflow.com/questions/2675362/how-to-find-an-available-port

		try {
			this.sServ = new ServerSocket(0);

			int port = this.sServ.getLocalPort();
			int firstPart = port / 256;
			int second_part = port - firstPart * 256;

			if (address.equals("localhost")) {
				this.ipPortNumber = "0,0,0,0," + firstPart + "," + second_part;
				this.address = "0.0.0.0";
			} else {
				String[] parseAddress = address.split(".");
				this.ipPortNumber =
					"" + parseAddress[0] + "," + parseAddress[1] +
					"," + parseAddress[2] + "," + parseAddress[3] +
					"," + firstPart + "," + second_part;
				this.address = address;
			}
		} catch (IOException err) {
			err.printStackTrace();
		}
	}


	public void setPortPasive(String command) {
		String[] cmdSplit = command.split(" ");
		String[] parts = cmdSplit[cmdSplit.length - 1].trim().split(",");
		int firstPart = Integer.parseInt(parts[parts.length - 2]);
		int secondPart = Integer.parseInt(parts[parts.length - 1].split("\\)")[0]);
		this.port = firstPart * 256 + secondPart;
	}

	public void startPasive() throws IOException {
		sCon = new Socket(this.address, this.port);
		input = new DataInputStream(sCon.getInputStream());	   
		output = new DataOutputStream(sCon.getOutputStream());
	}

	public void acceptConnection() throws IOException {
		this.sCon = this.sServ.accept();
		input = new DataInputStream(sCon.getInputStream());	   
		output = new DataOutputStream(sCon.getOutputStream());
	}

	public void closeConnection() throws IOException {
		this.sCon.close();
	}

	public String listFile() throws IOException {
		BufferedReader in = new BufferedReader(
			new InputStreamReader(sCon.getInputStream())
		);
		String data = "";
		while (true) {
			try {
				String s = in.readLine();
				if (s != null) {
					data += s + "\n";
				} else {
					break;
				}
			} catch (IOException err) {
				break;
			}
		}
		sCon.close();
		return data;
	}

	public byte[] downloadFile() throws IOException {
		int dataLen = 0;
		ArrayList<Byte> list = new ArrayList<Byte>();

		while (dataLen != -1) {
			byte[] data = new byte[1024]; 
        	
			dataLen = input.read(data);
			for (int i = 0; i < dataLen; i++) {
				list.add(data[i]);
			}
		}

		byte[] finalData = new byte[list.size()];
		for (int i = 0; i < finalData.length; i++) {
			finalData[i] = list.get(i);
		}
			
		sCon.close();
		return finalData;
	}

	public void uploadFile(byte[] data) throws IOException {
		// https://stackoverflow.com/questions/1176135/socket-send-and-receive-byte-array

		output.write(data);
		sCon.close();
	}


	public void storeDataFile(byte[] data, String filename) throws IOException {
		// https://examples.javacodegeeks.com/core-java/io/fileoutputstream/write-byte-array-to-file-with-fileoutputstream/
		
		File file = new File(filename);
		FileOutputStream fos = new FileOutputStream(file);
		fos.write(data);
		fos.close();
	}

	public byte[] getDataFile(String filename) throws IOException {
		// https://stackoverflow.com/questions/858980/file-to-byte-in-java
		
		File file;
		while (true) {
			file = new File(filename);
			if (file.exists()) {
				break;
			} else {
				BufferedReader inputKeyboard =
					new BufferedReader(new InputStreamReader(System.in));
				System.out.print("ERROR! File do not exist\nfilename: ");
				System.out.flush();
				filename = inputKeyboard.readLine(); 
			}
		}
		return Files.readAllBytes(file.toPath());
	}


	public String getIpPort() {
		return this.ipPortNumber;
	}
}
