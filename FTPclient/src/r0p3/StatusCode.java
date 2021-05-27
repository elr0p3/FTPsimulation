package r0p3;

public class StatusCode {

	public static final int NONE = -1;
	public static final int ERROR = 0;
	public static final int OK = 1;
	public static final int EXIT = 2;
	public static final int DOWN = 3;
	public static final int UP = 4;
	public static final int LIST = 5;
	public static final int MOVE = 6;


	public int interpretStatusCode(String command, String response) {

		// Command.INITIATE
		// Command.PASIVE
		// Command.RNAME_FR
		// Command.RNAME_TO


		if (command.startsWith(Command.USER)) {	//////////////////////////////
			if (response.startsWith("331")) {
				return StatusCode.OK;
			}
			return StatusCode.OK;
		
		} else if (command.startsWith(Command.PASSWD)) {	//////////////////
			if (response.startsWith("230")) {
				return StatusCode.OK;
			}
			// else 530
			return StatusCode.ERROR;
		
		} else if (command.startsWith(Command.LS)) {	//////////////////////
			if (response.startsWith("150")) {
				// 226 OK
				// 425 || 451 ERROR
				return StatusCode.LIST;
			}
			// else 450 || 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.GET)) {	//////////////////////
			if (response.startsWith("150")) {
				// 226 OK
				// 425 || 426 || 451 ERROR
				return StatusCode.DOWN;
			}
			// else 450 || 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.PUT)) {	//////////////////////
			if (response.startsWith("150")) {
				// 226 OK
				// 425 || 426 || 451 ERROR
				return StatusCode.UP;
			}
			// else 450 || 452 || 553
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.QUIT)) {	//////////////////////
			if (response.startsWith("225")) {
				return StatusCode.EXIT;
			}
			return StatusCode.EXIT;

		} else if (command.startsWith(Command.PWD)) {	//////////////////////
			if (response.startsWith("257")) {
				return StatusCode.OK;
			}
			return StatusCode.OK;

		} else if (command.startsWith(Command.CD)) {	//////////////////////
			if (response.startsWith("257")) {
				return StatusCode.OK;
			}
			// else 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.MKDIR)) {	//////////////////////
			if (response.startsWith("257")) {
				return StatusCode.OK;
			}
			// else 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.RMDIR)) {	//////////////////////
			if (response.startsWith("250")) {
				return StatusCode.OK;
			}
			// else 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.DELETE)) {	//////////////////
			if (response.startsWith("250")) {
				return StatusCode.OK;
			}
			// else 450 || 550
			return StatusCode.ERROR;

		} else if (command.startsWith(Command.RNAME_FR)) {	//////////////////
			if (response.startsWith("350")) {
				return StatusCode.MOVE;
			}
			// 450 || 550
			return StatusCode.ERROR;
		// } else if (command.startsWith(Command.RNAME_TO)) {	//////////////////
		}
		return StatusCode.NONE;
	}


	public int interpretList(String command) {
		if (command.startsWith("226")) {
			return StatusCode.OK;
		}
		// 425 || 451 ERROR
		return StatusCode.ERROR;
	}


	public int interpretFile(String command) {
		if (command.startsWith("226")) {
			return StatusCode.OK;
		}
		// 425 || 426 || 451 ERROR
		return StatusCode.ERROR;
	}
}
