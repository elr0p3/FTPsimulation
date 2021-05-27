package r0p3;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;

public class Command {

	public static final String INITIATE	= "PORT";
	public static final String PASIVE	= "PASV";
	public static final String USER 	= "USER";
	public static final String PASSWD	= "PASS";

	public static final String LS   	= "LIST";
	public static final String GET  	= "RETR";
	public static final String PUT  	= "STOR";
	public static final String QUIT 	= "QUIT";
	public static final String PWD  	= "PWD";
	public static final String CD   	= "CWD";
	public static final String MKDIR	= "MKD";
	public static final String RMDIR	= "RMD";
	public static final String DELETE	= "DELE";

	public static final String RNAME_FR	= "RNFR";
	public static final String RNAME_TO	= "RNTO";



	private static final String initiate	= "PORT %s\r\n";
	private static final String pasive	= "PASV\r\n";
	private static final String user 	= "USER %s\r\n";
	private static final String passwd	= "PASS %s\r\n";

	private static final String ls   	= "LIST\r\n";
	private static final String ls_spe	= "LIST %s\r\n";
	private static final String get  	= "RETR %s\r\n";
	private static final String put  	= "STOR %s\r\n";
	private static final String quit 	= "QUIT\r\n";
	private static final String pwd  	= "PWD\r\n";
	private static final String cd   	= "CWD %s\r\n";
	private static final String mkdir	= "MKD %s\r\n";
	private static final String rmdir	= "RMD %s\r\n";
	private static final String delete	= "DELE %s\r\n";

	private static final String rname_fr	= "RNFR %s\r\n";
	private static final String rname_to	= "RNTO %s\r\n";


	private static final String help = """
Commands are:

  ?
  cd
  delete
  get
  ls 
  mkdir
  mv
  put
  pwd
  quit
  rmdir
""";



	public String selectMode(String portNumber) throws IOException {
		BufferedReader inputKeyboard =
			new BufferedReader(new InputStreamReader(System.in));
		System.out.print("mode [A/p]: ");
		System.out.flush();
		String input_cmd = inputKeyboard.readLine();

		if (input_cmd.equals("P") || input_cmd.equals("p")) {
			return Command.pasive;
		} else {
			return String.format(Command.initiate, portNumber);
		}
	}


	public String inputUserName() throws IOException {
		BufferedReader inputKeyboard =
			new BufferedReader(new InputStreamReader(System.in));
		System.out.print("username: ");
		System.out.flush();
		String input_cmd = inputKeyboard.readLine();
		return String.format(Command.user, input_cmd);
	}

	public String inputPasswd() throws IOException {
		BufferedReader inputKeyboard =
			new BufferedReader(new InputStreamReader(System.in));
		System.out.print("password: ");
		System.out.flush();
		String input_cmd = inputKeyboard.readLine();
		return String.format(Command.passwd, input_cmd);
	}

	public String inputData(String scope) throws IOException {
		BufferedReader inputKeyboard =
			new BufferedReader(new InputStreamReader(System.in));
		System.out.print(scope + ": ");
		System.out.flush();
		return inputKeyboard.readLine();
	}

	public String renameTo(String name) {
		return String.format(Command.rname_to, name);
	}



	public String inputCommand() throws IOException {
		BufferedReader inputKeyboard =
			new BufferedReader(new InputStreamReader(System.in));

		while (true) {
			System.out.print("input: ");
			System.out.flush();
			String input_cmd = inputKeyboard.readLine();

			if (input_cmd.equals("?")) {	//////////////////////////////////
				System.out.println(Command.help);
				continue;

			} else if (input_cmd.startsWith("cd")) {	//////////////////////
				String[] c = input_cmd.split(" ");
				if (c.length == 2)
					return String.format(Command.cd, c[1]);
				else {
					System.err.println("Invalid directory input");
					continue;
				}

			} else if (input_cmd.startsWith("delete")) {	//////////////////
				String[] c = input_cmd.split(" ");
				if (c.length == 2)
					return String.format(Command.delete, c[1]);
				else {
					System.err.println("Invalid file or directory input");
					continue;
				}

			} else if (input_cmd.startsWith("get")) {	//////////////////////
				String filename = this.inputData("remote");
				return String.format(Command.get, filename);

			} else if (input_cmd.equals("ls")) {	/////////////////////////////////
				return Command.ls;

			} else if (input_cmd.startsWith("ls")) {	//////////////////////
				String[] c = input_cmd.split(" ");
				if (c.length == 2)
					return String.format(Command.ls_spe, c[1]);
				else {
					System.err.println("Invalid directory input");
					continue;
				}

			} else if (input_cmd.startsWith("mkdir")) {	//////////////////////
				String[] c = input_cmd.split(" ");
				if (c.length == 2)
					return String.format(Command.mkdir, c[1]);
				else {
					System.err.println("Invalid directory input");
					continue;
				}

			} else if (input_cmd.startsWith("put")) {	//////////////////////
				String filename = this.inputData("remote");
				return String.format(Command.put, filename);


			} else if (input_cmd.equals("pwd")) {	//////////////////////////////
				return Command.pwd;

			} else if (input_cmd.equals("quit")) {	//////////////////////////////
				return Command.quit;

			} else if (input_cmd.startsWith("rmdir")) {	//////////////////////
				String[] c = input_cmd.split(" ");
				if (c.length == 2)
					return String.format(Command.rmdir, c[1]);
				else {
					System.err.println("Invalid directory input");
					continue;
				}

			} else if (input_cmd.startsWith("mv")) {
				String name = this.inputData("actual");
				return String.format(Command.rname_fr, name);

			} else {
				System.err.println("Invalid command!");
				continue;
			}
		}
		// return "MARIKONG";
	}
}
