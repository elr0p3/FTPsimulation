package networking;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.ServerSocketChannel;
import java.nio.channels.SocketChannel;
import java.util.Iterator;
import java.util.Set;


// TODO
interface Listener {
	void connection(Selector selector, ServerSocketChannel listener, SocketChannel client);
	
	void write();
	
	void read();
}


public class SocketServer {
	public static int DEFAULT_PORT = 7;
	 
	public void SelectorStart() {
		 int port;
		 
		 port = DEFAULT_PORT;
		 
		 System.out.println("Listening for connections on port " + port);
		 
		 ServerSocketChannel serverChannel;
		 
		 Selector selector;
	
		 try {
			 serverChannel = ServerSocketChannel.open();
			 ServerSocket ss = serverChannel.socket();
			 InetSocketAddress address = new InetSocketAddress(port);
			 ss.bind(address);
			 serverChannel.configureBlocking(false);
			 selector = Selector.open();
			 serverChannel.register(selector, SelectionKey.OP_ACCEPT);
		 } catch (IOException ex) {
			 ex.printStackTrace();
			 return;
		 }
		 
		 while (true) {
			 try {			 
				 selector.select();
			 } catch (IOException ex) {
				 ex.printStackTrace();
				 break;
			 }
			 
			 Set<SelectionKey> readyKeys = selector.selectedKeys();
			 Iterator<SelectionKey> iterator = readyKeys.iterator();
			 
			 
			 while (iterator.hasNext()) {
				 SelectionKey key = iterator.next();
				 iterator.remove();
				 try {
					 if (key.isAcceptable()) {
						 ServerSocketChannel server = (ServerSocketChannel) key.channel();
						 SocketChannel client = server.accept();
						 System.out.println("Accepted connection from " + client);
						 client.configureBlocking(false);
						 SelectionKey clientKey = client.register(selector, SelectionKey.OP_WRITE | SelectionKey.OP_READ);						 
						 ByteBuffer buffer = ByteBuffer.allocate(1024);
						 clientKey.attach(buffer);
					 }
					 if (key.isReadable()) {
						 SocketChannel client = (SocketChannel) key.channel();						
						 ByteBuffer output = (ByteBuffer) key.attachment();					 
						 int bytesRead = client.read(output);
					 }
					 if (key.isWritable()) {
						 SocketChannel client = (SocketChannel) key.channel();
						 ByteBuffer output = (ByteBuffer) key.attachment();
						 output.flip();
						 client.write(output);
						 output.compact();
					 }
				 } catch (IOException ex) {
					 key.cancel();
					 try {
					 key.channel().close();
					 } catch (IOException cex) {}
				 }
			 }
		}	
	}
	 
	private static Selector open() {
		// TODO Auto-generated method stub
		return null;
	}
}
